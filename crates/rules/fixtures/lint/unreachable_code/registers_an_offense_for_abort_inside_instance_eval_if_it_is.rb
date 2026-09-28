class Dummy
  def abort; end
end

d = Dummy.new
d.instance_eval do
  Kernel.abort
  foo
  ^^^ Unreachable code detected.
end
