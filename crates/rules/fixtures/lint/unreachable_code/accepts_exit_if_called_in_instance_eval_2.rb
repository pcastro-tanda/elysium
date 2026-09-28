class Dummy
  def exit!; end
end

d = Dummy.new
d.instance_eval do
  exit!
  bar
end
