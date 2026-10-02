def test(&blk)
  blk ||= proc {}
  super(&blk)
end
