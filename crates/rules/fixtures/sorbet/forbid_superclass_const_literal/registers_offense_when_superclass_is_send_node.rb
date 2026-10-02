class MyClass < Struct.new(:foo, :bar, :baz); end
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidSuperclassConstLiteral: Superclasses must only contain constant literals
