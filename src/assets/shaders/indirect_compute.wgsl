struct ChunkIndirectMeta {
    instance_count: u32,
    first_instance: u32,
    _pad0: u32,
    _pad1: u32,
};

struct IndexedIndirectMeshParams {
    index_count: u32,
    first_index: u32,
    base_vertex: i32,
    _pad0: u32,
};

struct DrawIndexedIndirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
};

@group(0) @binding(0)
var<storage, read> chunk_meta: array<ChunkIndirectMeta>;

@group(0) @binding(1)
var<uniform> mesh: IndexedIndirectMeshParams;

@group(0) @binding(2)
var<storage, read_write> output_commands: array<DrawIndexedIndirect>;

@compute @workgroup_size(64, 1, 1)
fn build_indexed_indirect(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= arrayLength(&chunk_meta)) {
        return;
    }

    let meta = chunk_meta[i];
    output_commands[i] = DrawIndexedIndirect(
        mesh.index_count,
        meta.instance_count,
        mesh.first_index,
        mesh.base_vertex,
        meta.first_instance,
    );
}
