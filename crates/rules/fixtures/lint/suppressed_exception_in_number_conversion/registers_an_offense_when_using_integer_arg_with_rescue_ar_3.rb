begin
^^^^^ Use `Integer(arg, exception: false)` instead.
  Integer(arg)
rescue ::ArgumentError, ::TypeError
  nil
end
