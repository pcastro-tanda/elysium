module Foo
  class Exception < RuntimeError; end
  class C < ::Exception; end
            ^^^^^^^^^^^ Inherit from `StandardError` instead of `Exception`.
end
