class Dummy
  def abort; end
end

d = Dummy.new
d.instance_eval do
  d2 = Dummy.new
  d2.instance_eval do
    abort
    bar
  end
end
