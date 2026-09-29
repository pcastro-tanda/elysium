def self.my_method(x, y = 42, *args, **options)
  super { yield }
  ^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.
end
