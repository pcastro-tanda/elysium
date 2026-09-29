def m(&blk)
  items.something { |i| yield i }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.
end
