"foo".send(:to_c)
^^^^^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `:to_c`, use stricter `{ |i| Complex(i) }`.
