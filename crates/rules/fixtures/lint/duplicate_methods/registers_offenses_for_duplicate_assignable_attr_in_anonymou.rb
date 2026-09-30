Class.new do
  attr :something, true

  def something
  ^^^^^^^^^^^^^ Method `Object#something` is defined at both example.rb:2 and example.rb:4.
  end
  def something=(right)
  ^^^^^^^^^^^^^^ Method `Object#something=` is defined at both example.rb:2 and example.rb:6.
  end
end
