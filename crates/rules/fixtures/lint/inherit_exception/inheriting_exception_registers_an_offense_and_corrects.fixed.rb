module Foo
  class C < RuntimeError; end # This `Exception` is the same as `::Exception`.
  class Exception < RuntimeError; end
end
