class A
  def some_method
    implement 1
  end
  def some_method
  ^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both scripts/import.rb:2 and scripts/import.rb:5.
    implement 2
  end
end
