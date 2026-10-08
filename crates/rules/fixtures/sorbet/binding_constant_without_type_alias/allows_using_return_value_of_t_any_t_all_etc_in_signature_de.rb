sig { params(foo: T.any(String, Integer)).void }
def a(foo); end

sig { params(foo: T.all(String, Integer)).void }
def b(foo); end

sig { returns(T.noreturn) }
def c; end

sig { params(foo: T.class_of(String)).void }
def d(foo); end

sig { params(foo: T.proc.void).void }
def e(foo); end

sig { params(foo: T.untyped).void }
def f(foo); end

sig { params(foo: T.nilable(String)).void }
def g(foo); end

sig { params(foo: T.self_type).void }
def h(foo); end
