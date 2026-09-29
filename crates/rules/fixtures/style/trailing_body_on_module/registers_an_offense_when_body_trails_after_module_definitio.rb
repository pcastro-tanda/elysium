module Foo body
           ^^^^ Place the first line of module body on its own line.
end
module Bar extend self
           ^^^^^^^^^^^ Place the first line of module body on its own line.
end
module Bar; def bar; end
            ^^^^^^^^^^^^ Place the first line of module body on its own line.
end
module Bar def bar; end
           ^^^^^^^^^^^^ Place the first line of module body on its own line.
end
