module Foo
  include Bar

  class C < Exception; end # This `Exception` is the same as `::Exception`.
            ^^^^^^^^^ Inherit from `RuntimeError` instead of `Exception`.
  class Exception < RuntimeError; end
end
