def test(&blk)
  if foo
    blk = proc {} if bar
  end
  super(&blk)
end
