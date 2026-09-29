def my_method
  super() { yield }
  ^^^^^^^^^^^^^^^^^ Consider using explicit block argument in the surrounding method's signature over `yield`.
end
