A.class_eval do
  def some_method
    implement 1
  end
  def some_method
  ^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both dups.rb:2 and dups.rb:5.
    implement 2
  end
  def any_method
    implement 1
  end
  def any_method
  ^^^^^^^^^^^^^^ Method `A#any_method` is defined at both dups.rb:8 and dups.rb:11.
    implement 2
  end
end
