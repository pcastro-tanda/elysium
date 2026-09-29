def some_method; body
                 ^^^^ Place the first line of a multi-line method definition's body on its own line.
end
def extra_large; { size: 15 };
                 ^^^^^^^^^^^^ Place the first line of a multi-line method definition's body on its own line.
end
def seven_times(stuff) 7.times { do_this(stuff) }
                       ^^^^^^^^^^^^^^^^^^^^^^^^^^ Place the first line of a multi-line method definition's body on its own line.
end
