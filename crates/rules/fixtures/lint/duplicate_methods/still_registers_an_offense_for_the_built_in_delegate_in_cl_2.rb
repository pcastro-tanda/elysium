A.class_eval do
  def some_method
    implement 1
  end
  delegate :some_method, to: :foo
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both example.rb:2 and example.rb:5.
end
