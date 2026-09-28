module Foo
  class Exception < RuntimeError; end # This `Exception` is the same as `Foo::Exception`.
  class C < Exception; end
end
