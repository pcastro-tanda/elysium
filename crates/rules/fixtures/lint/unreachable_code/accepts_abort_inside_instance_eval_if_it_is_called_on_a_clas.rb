class Dummy
  def abort; end
end

d = Dummy.new
d.instance_eval do
  Dummy.abort
  foo
end
