Course.where('enrollments.student_id <> ?', student.id)
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where.not(enrollments: { student_id: student.id })` instead of manually constructing negated SQL in `where`.
