pub(crate) mod state;
use std::{future::Future, pin::Pin};

use crate::world::World;
use state::SystemState;

type AsyncUnit = Option<Pin<Box<dyn Future<Output = ()>>>>;

/// 函数系统 : 由实现了[FnSystemParm]特征的类型作为参数,并且加上
/// [proc::system]属性的的函数
pub trait InnerSystem<Marker> {
    /// 从[World]创建参数
    fn build_args(&self, world: &World) -> Box<()>;

    /// 初始化
    fn init(&self);

    fn run_once(&mut self, args: Box<()>) -> AsyncUnit;
}

/// 实现此特征 就可以作为[System]的参数
pub(crate) trait SystemParm {
    /// 从[World]创建
    ///
    /// # Safety
    ///
    /// 这个函数的安全性通过[FnSystemParm::init]保证
    unsafe fn build(world: &World) -> Self;

    /// 初始化,通过[SystemState]保证安全性
    fn init(state: &mut SystemState);
}

mod __impl {

    #[cfg(not(feature = "async"))]
    mod _normal {
        use super::super::*;
        macro_rules! impl_fnsystem {
        ($($t:ident),*) => {
            impl<F,$($t : SystemParm,)*> InnerSystem<($($t,)*)> for F
            where F : FnMut($($t,)*) {
                fn build_args(&self, world: &World) -> Box<()>{
                    unsafe{
                        std::mem::transmute(Box::new(($($t::build(world),)*)))
                    }
                }

                fn init(&self) {
                    let mut state = SystemState::new();
                    $($t::init(&mut state);)*
                }

                fn run_once(&mut self, args: Box<()>) -> AsyncUnit{
                    let ($($t,)*) = unsafe{
                        *std::mem::transmute::<_,Box<($($t,)*)>>(args)
                    };
                    (self)($($t,)*);
                    None
                }
            }
            };
        }
        trecs_proc::all_tuple!(impl_fnsystem, 16);
        impl<F> InnerSystem<()> for F
        where
            F: FnMut(),
        {
            fn build_args(&self, _world: &World) -> Box<()> {
                Box::new(())
            }

            fn init(&self) {}

            fn run_once(&mut self, _args: Box<()>) -> AsyncUnit {
                (self)();
                None
            }
        }
    }

    #[cfg(feature = "async")]
    mod _async {
        use super::super::*;
        macro_rules! impl_async_fnsystem {
        ($($t:ident),*) => {
            impl<F,R,$($t : SystemParm,)*> InnerSystem<($($t,)*)> for F
            where F : FnMut($($t,)*) -> R,
                  R: Future<Output = ()> + 'static,
            {
                fn build_args(&self, world: &World) -> Box<()>{
                    unsafe{
                        std::mem::transmute(Box::new(($($t::build(world),)*)))
                    }
                }

                fn init(&self) {
                    let mut state = SystemState::new();
                    $($t::init(&mut state);)*
                }

                fn run_once(&mut self, args: Box<()>) -> AsyncUnit{
                    let ($($t,)*) = unsafe{
                        *std::mem::transmute::<_,Box<($($t,)*)>>(args)
                    };
                    (self)($($t,)*);
                    None
                }
            }
            };
        }
        trecs_proc::all_tuple!(impl_async_fnsystem, 16);
        #[cfg(feature = "async")]
        impl<F, R> InnerSystem<()> for F
        where
            F: FnMut() -> R,
            R: Future<Output = ()> + 'static,
        {
            fn build_args(&self, _world: &World) -> Box<()> {
                Box::new(())
            }

            fn init(&self) {}

            fn run_once(&mut self, _args: Box<()>) -> AsyncUnit {
                Some(Box::pin((self)()))
            }
        }
    }
}

#[non_exhaustive]
pub enum System {
    #[cfg(not(feature = "async"))]
    Normal(Box<dyn InnerSystem<()>>),
    #[cfg(feature = "async")]
    Async(Box<dyn InnerSystem<()>>),
}

impl System {
    pub(crate) fn new<M, F: InnerSystem<M>>(fn_system: F) -> Self {
        fn_system.init();
        let fn_system: Box<dyn InnerSystem<M>> = Box::new(fn_system);

        let inner: Box<dyn InnerSystem<()>> = unsafe { std::mem::transmute(fn_system) };

        #[cfg(feature = "async")]
        return Self::Async(inner);
        #[cfg(not(feature = "async"))]
        return Self::Normal(inner);
    }

    #[cfg(not(feature = "async"))]
    pub(crate) fn run_once(&mut self, world: &World) {
        let System::Normal(inner) = self;
        inner.run_once(inner.build_args(world));
    }
    #[cfg(feature = "async")]
    pub(crate) async fn run_once(&mut self, world: &World) {
        let System::Async(inner) = self;
        inner.run_once(inner.build_args(world)).unwrap().await;
    }
}
