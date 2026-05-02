use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use miden_core::{Felt, ZERO, program::MIN_STACK_DEPTH};
use miden_processor::{ExecutionOptions, FastProcessor, advice::AdviceInputs};
use miden_vm::{Assembler, DefaultHost, StackInputs};

const INITIAL_STACK_BUFFER_SIZE: usize = 6850;
const INITIAL_STACK_TOP_IDX: usize = 250;
const STACK_BUFFER_BASE_IDX: usize = INITIAL_STACK_TOP_IDX - MIN_STACK_DEPTH;
const DEFAULT_LIMIT_PUSHES: usize = ExecutionOptions::DEFAULT_MAX_STACK_DEPTH - MIN_STACK_DEPTH;
const GROWTH_SCENARIOS: &[(usize, &str)] = &[
    (1024, "grow_1024_past_initial_buffer"),
    (4096, "grow_4096_past_initial_buffer"),
    (16384, "grow_16384_past_initial_buffer"),
];

fn pad_then_drop_program(pushes: usize) -> miden_vm::Program {
    let source = format!(
        "
        begin
            repeat.{pushes}
                push.0
            end
            repeat.{pushes}
                drop
            end
        end
        "
    );

    Assembler::default()
        .assemble_program(&source)
        .expect("failed to assemble stack growth benchmark program")
}

fn bench_program(c: &mut Criterion) {
    let mut group = c.benchmark_group("operand_stack_growth");
    group.sample_size(20);

    let no_growth_program = pad_then_drop_program(DEFAULT_LIMIT_PUSHES);
    group.bench_function("default_limit_no_growth", |bench| {
        bench.iter_batched(
            || {
                let host = DefaultHost::default();
                let processor = FastProcessor::new_with_options(
                    StackInputs::default(),
                    AdviceInputs::default(),
                    ExecutionOptions::default(),
                );

                (host, processor)
            },
            |(mut host, processor)| {
                let output = processor.execute_sync(&no_growth_program, &mut host).unwrap();
                black_box(output);
            },
            BatchSize::SmallInput,
        );
    });

    for &(growth_margin, name) in GROWTH_SCENARIOS {
        let growth_pushes = DEFAULT_LIMIT_PUSHES + growth_margin;
        let growth_program = pad_then_drop_program(growth_pushes);
        let growth_options = ExecutionOptions::default()
            .with_max_stack_depth(MIN_STACK_DEPTH + growth_pushes)
            .unwrap();

        group.bench_function(name, |bench| {
            bench.iter_batched(
                || {
                    let host = DefaultHost::default();
                    let processor = FastProcessor::new_with_options(
                        StackInputs::default(),
                        AdviceInputs::default(),
                        growth_options,
                    );

                    (host, processor)
                },
                |(mut host, processor)| {
                    let output = processor.execute_sync(&growth_program, &mut host).unwrap();
                    black_box(output);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

trait StackStorage {
    fn new(max_depth: usize) -> Self;
    fn push(&mut self, value: Felt);
    fn pop(&mut self) -> Felt;
}

struct BoxStack {
    stack: Box<[Felt]>,
    stack_top_idx: usize,
    stack_bot_idx: usize,
    max_depth: usize,
}

impl BoxStack {
    fn stack_size(&self) -> usize {
        self.stack_top_idx - self.stack_bot_idx
    }

    fn grow_stack_buffer(&mut self, min_len: usize) {
        let max_len = STACK_BUFFER_BASE_IDX + self.max_depth + 1;
        let live_len = self.stack_size();
        let required_len =
            min_len.min(max_len).max(STACK_BUFFER_BASE_IDX + live_len + 2);

        let mut new_len = self.stack.len();
        while new_len < required_len {
            new_len = new_len.saturating_mul(2).min(max_len);
        }

        let mut new_stack = vec![ZERO; new_len].into_boxed_slice();
        let new_stack_bot_idx = STACK_BUFFER_BASE_IDX;
        let new_stack_top_idx = new_stack_bot_idx + live_len;
        new_stack[new_stack_bot_idx..new_stack_top_idx]
            .copy_from_slice(&self.stack[self.stack_bot_idx..self.stack_top_idx]);

        self.stack = new_stack;
        self.stack_bot_idx = new_stack_bot_idx;
        self.stack_top_idx = new_stack_top_idx;
    }
}

impl StackStorage for BoxStack {
    fn new(max_depth: usize) -> Self {
        Self {
            stack: vec![ZERO; INITIAL_STACK_BUFFER_SIZE].into_boxed_slice(),
            stack_top_idx: INITIAL_STACK_TOP_IDX,
            stack_bot_idx: STACK_BUFFER_BASE_IDX,
            max_depth,
        }
    }

    fn push(&mut self, value: Felt) {
        assert!(self.stack_size() < self.max_depth);
        if self.stack_top_idx >= self.stack.len() - 1 {
            self.grow_stack_buffer(self.stack_top_idx + 2);
        }

        self.stack[self.stack_top_idx] = value;
        self.stack_top_idx += 1;
    }

    fn pop(&mut self) -> Felt {
        self.stack_top_idx -= 1;
        let value = self.stack[self.stack_top_idx];
        self.stack_bot_idx = self.stack_bot_idx.min(self.stack_top_idx - MIN_STACK_DEPTH);
        value
    }
}

struct VecStack {
    stack: Vec<Felt>,
    stack_top_idx: usize,
    stack_bot_idx: usize,
    max_depth: usize,
}

impl VecStack {
    fn stack_size(&self) -> usize {
        self.stack_top_idx - self.stack_bot_idx
    }

    fn grow_stack_buffer(&mut self, min_len: usize) {
        let max_len = STACK_BUFFER_BASE_IDX + self.max_depth + 1;
        let live_len = self.stack_size();
        let required_len =
            min_len.min(max_len).max(STACK_BUFFER_BASE_IDX + live_len + 2);

        let mut new_len = self.stack.len();
        while new_len < required_len {
            new_len = new_len.saturating_mul(2).min(max_len);
        }

        self.stack.resize(new_len, ZERO);

        let new_stack_bot_idx = STACK_BUFFER_BASE_IDX;
        let new_stack_top_idx = new_stack_bot_idx + live_len;
        self.stack
            .copy_within(self.stack_bot_idx..self.stack_top_idx, new_stack_bot_idx);
        self.stack[..new_stack_bot_idx].fill(ZERO);

        self.stack_bot_idx = new_stack_bot_idx;
        self.stack_top_idx = new_stack_top_idx;
    }
}

impl StackStorage for VecStack {
    fn new(max_depth: usize) -> Self {
        Self {
            stack: vec![ZERO; INITIAL_STACK_BUFFER_SIZE],
            stack_top_idx: INITIAL_STACK_TOP_IDX,
            stack_bot_idx: STACK_BUFFER_BASE_IDX,
            max_depth,
        }
    }

    fn push(&mut self, value: Felt) {
        assert!(self.stack_size() < self.max_depth);
        if self.stack_top_idx >= self.stack.len() - 1 {
            self.grow_stack_buffer(self.stack_top_idx + 2);
        }

        self.stack[self.stack_top_idx] = value;
        self.stack_top_idx += 1;
    }

    fn pop(&mut self) -> Felt {
        self.stack_top_idx -= 1;
        let value = self.stack[self.stack_top_idx];
        self.stack_bot_idx = self.stack_bot_idx.min(self.stack_top_idx - MIN_STACK_DEPTH);
        value
    }
}

fn run_storage_workload<S: StackStorage>(pushes: usize, max_depth: usize) -> usize {
    let mut stack = S::new(max_depth);

    for _ in 0..pushes {
        stack.push(black_box(ZERO));
    }
    for _ in 0..pushes {
        black_box(stack.pop());
    }

    pushes
}

fn bench_storage(c: &mut Criterion) {
    let mut group = c.benchmark_group("operand_stack_storage_growth");
    group.sample_size(20);

    let scenarios = core::iter::once((0, "default_limit_no_growth"))
        .chain(GROWTH_SCENARIOS.iter().copied());

    for (growth_margin, name) in scenarios {
        let pushes = DEFAULT_LIMIT_PUSHES + growth_margin;
        let max_depth = MIN_STACK_DEPTH + pushes;

        group.bench_with_input(BenchmarkId::new("box", name), &pushes, |bench, &pushes| {
            bench.iter(|| {
                let output = run_storage_workload::<BoxStack>(pushes, max_depth);
                black_box(output);
            });
        });

        group.bench_with_input(BenchmarkId::new("vec", name), &pushes, |bench, &pushes| {
            bench.iter(|| {
                let output = run_storage_workload::<VecStack>(pushes, max_depth);
                black_box(output);
            });
        });
    }

    group.finish();
}

criterion_group!(benchmark, bench_program, bench_storage);
criterion_main!(benchmark);
