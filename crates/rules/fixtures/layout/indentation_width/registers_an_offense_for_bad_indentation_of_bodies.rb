def my_func
  puts 'do something outside block'
  begin
  puts 'do something error prone'
  ^{} Use 2 (not 0) spaces for indentation.
  rescue SomeException, SomeOther => e
   puts 'wrongly indented error handling'
  ^ Use 2 (not 1) spaces for indentation.
  rescue
   puts 'wrongly indented error handling'
  ^ Use 2 (not 1) spaces for indentation.
  else
     puts 'wrongly indented normal case handling'
  ^^^ Use 2 (not 3) spaces for indentation.
  ensure
      puts 'wrongly indented common handling'
  ^^^^ Use 2 (not 4) spaces for indentation.
  end
end
