puts(FOO ? FOO.bar : nil)
     ^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

results << (FOO ? FOO.bar : nil)
            ^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
