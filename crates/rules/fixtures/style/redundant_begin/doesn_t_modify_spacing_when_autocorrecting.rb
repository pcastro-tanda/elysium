def method
  begin
  ^^^^^ Redundant `begin` block detected.
    BlockA do |strategy|
      foo
    end

    BlockB do |portfolio|
      foo
    end

  rescue => e # some problem
    bar
  end
end
