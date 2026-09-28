class Dummy
  def exit; end
end

d = Dummy.new
d.instance_eval do
  Kernel.exit
  foo
  ^^^ Unreachable code detected.
end
