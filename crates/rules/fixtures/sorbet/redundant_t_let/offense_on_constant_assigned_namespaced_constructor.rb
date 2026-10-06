MUTEX = T.let(Thread::Mutex.new.freeze, Thread::Mutex)
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The constant type is inferred from the constructor.
