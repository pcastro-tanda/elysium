def a_method
  super do |arg|
    foo
  rescue => e
    bar
  end
end
