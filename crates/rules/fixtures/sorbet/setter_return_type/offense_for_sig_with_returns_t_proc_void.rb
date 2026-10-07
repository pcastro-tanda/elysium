sig { params(name: String).returns(T.proc.void) }
                           ^^^^^^^^^^^^^^^^^^^^ Setter methods must declare a `void` return type.
def name=(name); end
