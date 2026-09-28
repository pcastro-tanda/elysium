class Dummy
  def fail; end
end

d = Dummy.new
d.instance_eval do
  Kernel.fail
  foo
  ^^^ Unreachable code detected.
end
