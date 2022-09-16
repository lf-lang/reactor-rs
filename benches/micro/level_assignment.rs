use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use reactor_rt::internals::*;

mod benchutils {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use array_macro::array;

    use index_vec::Idx;
    use reactor_rt::internals::*;
    use reactor_rt::assembly::*;
    use reactor_rt::{LocalReactionId, ReactorId};

    pub struct TestGraphFixture {
        graph: DepGraph,
        next_trigger_id: TriggerId,
        next_reactor_id: ReactorId,
        debug_info: DebugInfoRegistry,
    }

    impl TestGraphFixture {
        fn new() -> Self {
            Self {
                graph: DepGraph::new(),
                next_trigger_id: TriggerId::FIRST_REGULAR,
                debug_info: DebugInfoRegistry::new(),
                next_reactor_id: ReactorId::new(0),
            }
        }

        fn new_reactor(&mut self, name: impl Into<String>) -> TestAssembler {
            let reactor_id = self.next_reactor_id.get_and_incr();
            self.debug_info.record_reactor(reactor_id, ReactorDebugInfo::test_named(name));
            TestAssembler {
                reactor_id,
                first_trigger_id: self.next_trigger_id,
                fixture: self,
            }
        }

        pub fn number_reactions_by_level(&self) -> HashMap<GlobalReactionId, LevelIx> {
            self.graph
                .number_reactions_by_level()
                .map_err(|e| e.lift(&self.debug_info))
                .unwrap()
        }
        pub fn number_reactions_by_level_cpp(&self) -> HashMap<GlobalReactionId, LevelIx> {
            self.graph
                .number_reactions_by_level_cpp()
                .map_err(|e| e.lift(&self.debug_info))
                .unwrap()
        }
        pub fn number_reactions_by_level_old(&self) -> HashMap<GlobalReactionId, LevelIx> {
            self.graph
                .number_reactions_by_level_old()
                .map_err(|e| e.lift(&self.debug_info))
                .unwrap()
        }
    }

    struct TestAssembler<'a> {
        fixture: &'a mut TestGraphFixture,
        reactor_id: ReactorId,
        first_trigger_id: TriggerId,
    }

    impl TestAssembler<'_> {
        fn new_reactions<const N: usize>(&mut self) -> [GlobalReactionId; N] {
            let result = array![i => GlobalReactionId::new(self.reactor_id, LocalReactionId::from_usize(i)); N];
            let mut last = None;
            for n in &result {
                self.fixture.graph.record_reaction(*n);
                if let Some(last) = last {
                    self.fixture.graph.reaction_priority(last, *n);
                }
                last = Some(*n);
            }
            result
        }

        fn new_ports<const N: usize>(&mut self, names: [&'static str; N]) -> [TriggerId; N] {
            let result = array![_ => self.fixture.next_trigger_id.get_and_incr().unwrap(); N];
            for (i, p) in (&result).iter().enumerate() {
                self.fixture.graph.record_port(*p);
                self.fixture.debug_info.record_trigger(*p, Cow::Borrowed(names[i]));
            }
            result
        }
    }

    impl Drop for TestAssembler<'_> {
        fn drop(&mut self) {
            let range = self.first_trigger_id..self.fixture.next_trigger_id;
            self.fixture.debug_info.set_id_range(self.reactor_id, range)
        }
    }

    fn new_reaction(a: ReactorIdImpl, b: ReactionIdImpl) -> GlobalReactionId {
        GlobalReactionId::new(ReactorId::new(a), LocalReactionId::new(b))
    }

    pub fn make_test(size: u32) -> TestGraphFixture {
        let mut test = TestGraphFixture::new();
        let mut builder = test.new_reactor("top");
        let [mut prev_in] = builder.new_ports(["in"]);
        drop(builder);

        // the number of paths in the graph is exponential
        // in this upper bound, here 3^60.
        for reactor_id in 0..size {
            let mut builder = test.new_reactor(format!("r[{}]", reactor_id));
            let [n1, n2] = builder.new_reactions();
            let [p0, p1, out] = builder.new_ports(["p0", "p1", "out"]);
            drop(builder);

            // make a diamond
            test.graph.reaction_effects(n1, p0);
            test.graph.reaction_effects(n1, p1);
            test.graph.triggers_reaction(p0, n2);
            test.graph.triggers_reaction(p1, n2);

            // connect to prev_in
            test.graph.triggers_reaction(prev_in, n1);
            // replace prev_in with out
            test.graph.reaction_effects(n2, out);
            prev_in = out;
        }

        test
    }
}


fn bench_gid(c: &mut Criterion) {
    let mut group = c.benchmark_group("Level assignment");

    for size in [10, 20, 30, 60, 120] {
        let test = benchutils::make_test(size);
        group.bench_with_input(BenchmarkId::new("old", size), &test, |b, i| {
            b.iter(|| i.number_reactions_by_level_old())
        });
        group.bench_with_input(BenchmarkId::new("new", size), &test, |b, i| {
            b.iter(|| i.number_reactions_by_level())
        });
        group.bench_with_input(BenchmarkId::new("cpp", size), &test, |b, i| {
            b.iter(|| i.number_reactions_by_level_cpp())
        });
    }
    group.finish();
}

criterion_group!(benches, bench_gid);
criterion_main!(benches);
