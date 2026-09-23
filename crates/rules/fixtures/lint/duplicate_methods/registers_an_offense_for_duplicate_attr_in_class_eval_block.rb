A.class_eval do
  def something
  end
  attr :something
  ^^^^^^^^^^^^^^^ Method `A#something` is defined at both example.rb:2 and example.rb:4.
end
