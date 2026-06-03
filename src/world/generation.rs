use noise::{NoiseFn, SuperSimplex};

use raylib::prelude::*;
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::render::mesh_tools::VecMesh;
use crate::render::worldmesh;
use crate::world::blocks::BlockData;

pub const CHUNK_SIZE: i64 = 32;

pub struct ChunkGenThread {
    input_tx: Sender<(i64, i64, i64, Option<Chunk>)>,
    result_rx: Receiver<(i64, i64, i64, Option<Chunk>, VecMesh)>,
    chunk_gen_thread: JoinHandle<()>,

    chunks_in_progress: HashSet<(i64, i64, i64)>,
}

impl ChunkGenThread {
    pub fn new(seed: u32) -> Self {
        let (input_tx, input_rx) = mpsc::channel::<(i64, i64, i64, Option<Chunk>)>();
        let (result_tx, result_rx) = mpsc::channel::<(i64, i64, i64, Option<Chunk>, VecMesh)>();
        let chunk_gen_thread = thread::spawn(move || {
            ChunkGenThread::remote_generate_terrain_chunk(seed, input_rx, result_tx);
        });

        Self {
            input_tx,
            result_rx,
            chunk_gen_thread,
            chunks_in_progress: HashSet::new(),
        }
    }

    /// Dispatches a new chunk gen request
    pub fn dispatch_chunk_gen(&mut self, cx: i64, cy: i64, cz: i64) {
        if self.chunks_in_progress.contains(&(cx, cy, cz)) {
            return;
        }

        self.input_tx.send((cx, cy, cz, None)).unwrap();
        self.chunks_in_progress.insert((cx, cy, cz));
    }

    /// Dispatches request to only update mesh without generating chunk
    pub fn dispatch_mesh_chunk(&self, chunk: &Chunk, cx: i64, cy: i64, cz: i64) {
        // FIXME: unnecessary? move notice to top and delete
        if self.chunks_in_progress.contains(&(cx, cy, cz)) {
            return;
        }

        // FIXME: the clone here is almost certainly not the best idea, but it
        // functions as a poor (rich?) man's mutex and also prevents us from
        // having to contend with lifetimes
        self.input_tx.send((cx, cy, cz, Some(chunk.clone()))).unwrap();

        // XXX: We want to **allow** the same chunk to be sent for remeshing
        // multiple times, since the mpsc acts as a queue letting the thread
        // know that the chunk has changed. Otherwise, the mesh can lag behind
        // the current state of the chunk of the mesh update is too slow.
        // self.chunks_in_progress.insert((cx, cy, cz));
    }

    /// Polls the chunk gen thread for new blocks
    pub fn poll(&mut self) -> Option<(i64, i64, i64, Option<Chunk>, VecMesh)> {
        let result = self.result_rx.try_recv();
        match result {
            Ok(result) => {
                self.chunks_in_progress
                    .remove(&(result.0, result.1, result.2));
                Some(result)
            }
            Err(_) => {
                // we don't really care if it's disconnected or empty
                // although it would probably be good to log it
                // maybe in the future then
                None
            }
        }
    }

    /// Join this chunk generation thread
    pub fn join(self) -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
        // Drop input_tx to hang up and signal thread to terminate
        std::mem::drop(self.input_tx);
        self.chunk_gen_thread.join()
    }

    fn remote_generate_terrain_chunk(
        seed: u32,
        input_rx: Receiver<(i64, i64, i64, Option<Chunk>)>,
        result_tx: Sender<(i64, i64, i64, Option<Chunk>, VecMesh)>,
    ) {
        eprintln!("Terrain generation chunk started");
        loop {
            let Ok((cx, cy, cz, existing_chunk)) = input_rx.recv() else {
                eprintln!("chunk gen thread: input channel hung up, goodbye.");
                return;
            };

            if let Some(chunk) = existing_chunk {
                // Chunk was provided, only build the mesh
                let vmesh = worldmesh::remote_build_geometry_chunk(&chunk, cx, cy, cz);
                result_tx.send((cx, cy, cz, None, vmesh)).unwrap();
            } else {
                // If no chunk was provided, then we need to generate one
                let mut chunk = Chunk::new(cx, cy, cz);

                let r = 0..CHUNK_SIZE;

                for z in r.clone() {
                    for x in r.clone() {
                        let (wx, wz) = (x + CHUNK_SIZE * cx, z + CHUNK_SIZE * cz);
                        ChunkGenThread::remote_generate_terrain_column(seed, &mut chunk, wx, wz, cy);
                    }
                }

                let vmesh = worldmesh::remote_build_geometry_chunk(&chunk, cx, cy, cz);
                result_tx.send((cx, cy, cz, Some(chunk), vmesh)).unwrap();
            }

            eprintln!("done with {cx}, {cy}, {cz}");
        }
    }

    fn remote_generate_terrain_column(seed: u32, chunk: &mut Chunk, x: i64, z: i64, cy: i64) {
        // Generates one column within a chunk
        let ssn = SuperSimplex::new(seed);

        // How shallow slopes are. Don't set below 16 or it will error.
        let noise_scale = 80.;

        let sample_point = [(x as f64 / noise_scale), (z as f64 / noise_scale)];

        // arbitrary constants, give a height map between 4*12 and 6*12
        let height = ((ssn.get(sample_point) + 5_f64) * 12_f64) as i64;

        for y in (CHUNK_SIZE * cy)..(CHUNK_SIZE * (cy + 1)) {
            let block_data = if y > height {
                BlockData::AIR
            } else if y == height {
                BlockData::GRASS
            } else if y > height - 3 {
                BlockData::DIRT
            } else if y > 4 {
                BlockData::STONE
            } else {
                BlockData::BEDROCK
            };

            chunk.set_block_data(x, y, z, block_data);
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Chunk {
    /* absolute chunk coordinates
     * 1 unit = CHUNK_SIZE blocks */
    cx: i64,
    cy: i64,
    cz: i64,

    /* always must have length CHUNK_SIZE ^ 3
     *
     * ordered by row (x), then by column (z), then by layer (y)!
     *
     * so when iterating, use
     * for (y):
     *   for (z):
     *     for (x): */
    voxels: Box<[BlockData]>,
}

#[derive(Serialize, Deserialize)]
pub struct World {
    pub chunks: HashMap<(i64, i64, i64), Chunk>,
}

impl Chunk {
    pub fn new(cx: i64, cy: i64, cz: i64) -> Self {
        let voxel_count = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
        let mut voxels = Vec::with_capacity(voxel_count as usize);

        for _ in 0..voxel_count {
            voxels.push(BlockData::AIR);
        }

        Self {
            cx,
            cy,
            cz,
            voxels: voxels.into_boxed_slice(),
        }
    }

    pub fn get_block_data(self: &Self, x: i64, y: i64, z: i64) -> BlockData {
        self.voxels[self.get_block_idx(x, y, z)]
    }

    pub fn set_block_data(self: &mut Self, x: i64, y: i64, z: i64, value: BlockData) {
        self.voxels[self.get_block_idx(x, y, z)] = value;
    }

    pub fn is_within_bounds(&self, x: i64, y: i64, z: i64) -> bool {
        (x >= self.cx * CHUNK_SIZE && y >= self.cy * CHUNK_SIZE && z >= self.cz * CHUNK_SIZE)
            && (x < (self.cx + 1) * CHUNK_SIZE
                && y < (self.cy + 1) * CHUNK_SIZE
                && z < (self.cz + 1) * CHUNK_SIZE)
    }

    fn get_block_idx(self: &Self, x: i64, y: i64, z: i64) -> usize {
        let (lx, ly, lz) = (
            x - self.cx * CHUNK_SIZE,
            y - self.cy * CHUNK_SIZE,
            z - self.cz * CHUNK_SIZE,
        );
        let idx = ly * CHUNK_SIZE * CHUNK_SIZE + lz * CHUNK_SIZE + lx;

        idx as usize
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    pub fn get_chunk_coords_of_block(x: i64, y: i64, z: i64) -> (i64, i64, i64) {
        (
            if x >= 0 { x / CHUNK_SIZE } else { (x + 1) / CHUNK_SIZE - 1 },
            if y >= 0 { y / CHUNK_SIZE } else { (y + 1) / CHUNK_SIZE - 1 },
            if z >= 0 { z / CHUNK_SIZE } else { (z + 1) / CHUNK_SIZE - 1 }
        )
    }

    /* returns BlockData { non_void: false } for blocks in chunks
     * that haven't been generated yet */
    pub fn get_block_data(self: &Self, x: i64, y: i64, z: i64) -> BlockData {
        let (cx, cy, cz) = World::get_chunk_coords_of_block(x, y, z);

        if let Some(chunk) = self.chunks.get(&(cx, cy, cz)) {
            chunk.get_block_data(x, y, z)
        } else {
            BlockData::AIR
        }
    }

    /* panics if used in a chunk that hasn't been generated yet */
    pub fn set_block_data(self: &mut Self, x: i64, y: i64, z: i64, value: BlockData) {
        let (cx, cy, cz) = World::get_chunk_coords_of_block(x, y, z);

        if let Some(chunk) = self.chunks.get_mut(&(cx, cy, cz)) {
            chunk.set_block_data(x, y, z, value)
        } else {
            panic!("set block data in a chunk that doesn't exist");
        }
    }
}
