class Dummy
  def exit; end
end

d = Dummy.new
d.instance_eval do
  Dummy.exit
  foo
end
