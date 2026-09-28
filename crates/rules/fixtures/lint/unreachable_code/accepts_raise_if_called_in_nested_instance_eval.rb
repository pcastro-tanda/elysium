class Dummy
  def raise; end
end

d = Dummy.new
d.instance_eval do
  d2 = Dummy.new
  d2.instance_eval do
    raise
    bar
  end
end
