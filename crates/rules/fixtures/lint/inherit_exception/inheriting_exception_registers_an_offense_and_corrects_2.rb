module Foo
  class Exception < RuntimeError; end
  class C < ::Exception; end
            ^^^^^^^^^^^ Inherit from `RuntimeError` instead of `Exception`.
end
