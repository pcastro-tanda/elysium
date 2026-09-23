hash1 = { a: 0,
     bb: 1,
     ^^^^^ Align the keys of a hash literal if they span more than one line.
           ccc: 2 }
           ^^^^^^ Align the keys of a hash literal if they span more than one line.
hash2 = { :a   => 0,
          ^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
  :bb  => 1,
  ^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
            :ccc  =>2 }
            ^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
hash3 = { 'a'   =>   0,
          ^^^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
               'bb'  => 1,
               ^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
    'ccc'  =>2 }
    ^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
