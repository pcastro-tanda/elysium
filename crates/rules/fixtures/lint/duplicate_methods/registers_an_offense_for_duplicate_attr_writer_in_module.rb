module A
  def something=(right)
  end
  attr_writer :something
  ^^^^^^^^^^^^^^^^^^^^^^ Method `A#something=` is defined at both example.rb:2 and example.rb:4.
end
