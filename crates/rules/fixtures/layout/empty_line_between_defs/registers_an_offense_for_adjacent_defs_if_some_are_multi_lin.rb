def a; end
def b; end
def c # Not a one-liner, so this is an offense.
^^^^^ Expected 1 empty line between method definitions; found 0.
end
def d; end # Also an offense since previous was multi-line:
^^^^^ Expected 1 empty line between method definitions; found 0.
