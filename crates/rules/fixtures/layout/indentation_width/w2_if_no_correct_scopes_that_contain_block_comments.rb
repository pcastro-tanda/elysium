module Foo
# The class has a block comment within, so it's not corrected.
class Bar
^{} Use 2 (not 0) spaces for indentation.
=begin
This is a nice long
comment
which spans a few lines
=end
# The method has no block comment inside,
# but it's within a class that has a block
# comment, so it's not corrected either.
def baz
^{} Use 2 (not 0) spaces for indentation.
do_something
^{} Use 2 (not 0) spaces for indentation.
end
end
end
