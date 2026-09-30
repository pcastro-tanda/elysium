good = foo.methA(:arg1, :arg2, options: :hash)
         .methB(
           :arg1,
           :arg2,
         )
         .methC

good = foo.methA(
           :arg1,
           :arg2,
           options: :hash,
         )
         .methB(
           :arg1,
           :arg2,
         )
         .methC
