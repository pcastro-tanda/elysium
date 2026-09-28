module Foo
  class C < StandardError; end # This `Exception` is the same as `::Exception`.
  class Exception < RuntimeError; end
end
