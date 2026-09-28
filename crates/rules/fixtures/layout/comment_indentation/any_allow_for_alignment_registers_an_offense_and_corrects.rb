 # comment 1
 # comment 2
 # comment 3
 ^^^^^^^^^^^ Incorrect indentation detected (column 1 instead of 0).
hash1 = { a: 0,
     # comment 4
     ^^^^^^^^^^^ Incorrect indentation detected (column 5 instead of 10).
          bb: 1,
          ccc: 2 }
  if a
  #
  ^ Incorrect indentation detected (column 2 instead of 4).
    b
  # this is accepted
  elsif aa
    # so is this
  elsif bb
#
^ Incorrect indentation detected (column 0 instead of 4).
  else
   #
   ^ Incorrect indentation detected (column 3 instead of 4).
  end
  case a
  # this is accepted
  when 0
    # so is this
  when 1
     #
     ^ Incorrect indentation detected (column 5 instead of 4).
    b
  when 2
    # this is also accepted
  end
  case a
  # this is accepted
  in 0
    # so is this
  in 1
  #
  ^ Incorrect indentation detected (column 2 instead of 4).
    b
  in 2
    # this is also accepted
  end
