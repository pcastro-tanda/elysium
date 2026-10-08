def do_something
  foo \
    || bar \
           ^ Redundant line continuation.
end
