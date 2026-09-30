good = foo.methA(:arg1, :arg2, options: :hash)
         .methB(
           :arg1,
           :arg2,
     )
     ^ Indent `)` to column 9 (not 5)
         .methC

good = foo.methA(
           :arg1,
           :arg2,
           options: :hash,
    )
    ^ Indent `)` to column 9 (not 4)
         .methB(
           :arg1,
           :arg2,
       )
       ^ Indent `)` to column 9 (not 7)
         .methC
