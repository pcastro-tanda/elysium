module Foo
  class Exception < StandardError; end # This `Exception` is the same as `Foo::Exception`.
  class C < Exception; end
end
