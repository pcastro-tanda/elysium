module Foo
  class C < Exception; end # This `Exception` is the same as `::Exception`.
            ^^^^^^^^^ Inherit from `StandardError` instead of `Exception`.
  class Exception < RuntimeError; end
end
