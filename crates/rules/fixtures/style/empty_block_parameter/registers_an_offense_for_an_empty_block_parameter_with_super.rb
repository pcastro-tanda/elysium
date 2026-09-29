def foo
  super { || do_something }
          ^^ Omit pipes for the empty block parameters.
end
