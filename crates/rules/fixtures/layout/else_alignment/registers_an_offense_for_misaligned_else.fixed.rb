def my_func
  puts 'do something outside block'
  begin
    puts 'do something error prone'
  rescue SomeException, SomeOther => e
    puts 'wrongly intended error handling'
  rescue
    puts 'wrongly intended error handling'
  else
    puts 'wrongly intended normal case handling'
  ensure
    puts 'wrongly intended common handling'
  end
end
