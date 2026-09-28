def method
  
    BlockA do |strategy|
      foo
    end

    BlockB do |portfolio|
      foo
    end

  rescue => e # some problem
    bar
  
end
