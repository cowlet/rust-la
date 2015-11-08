use std::cmp;
use num;
//use num::Zero;
use num::traits::{ Float, Signed };

use ApproxEq;
use Matrix;
use internalutil::{alloc_dirty_vec};

/// LU Decomposition.
///
/// Originally based on JAMA.
///
/// For an m-by-n matrix A with m >= n, the LU decomposition is an m-by-n
/// unit lower triangular matrix L, an n-by-n upper triangular matrix U,
/// and a permutation vector piv of length m so that A(piv,:) = L*U.
/// If m < n, then L is m-by-m and U is m-by-n.
///
/// The LU decompostion with pivoting always exists, even if the matrix is
/// singular. The primary use of the LU decomposition is in the solution of
/// square systems of simultaneous linear equations. This will fail if the
/// matrix is singular.
//
// LU Decomposition:
// A = LU,
//   where L is a lower triangular matrix
//   and U is an upper triangular matrix.
//
// Step 1: Set diagonals of L to 1:
// Step 2: Solve columns of L and U in order from left to right.
//         For a specific column, solve in order from top to bottom, first solving the column for U and then for L.
//         For solving element e[i][j], write out the dot between i:th row of L and j:th column of U. By following
//         this specific order, you'll notice that by the time you are solving for that specific element, you have
//         already solved all the other variables in the equation, thus you just need to plug in the numbers to
//         solve the element.
//
//  Example: 3x3 matrix LU decomposition:
//    l11   0   0   u11 u12 u13   a11 a12 a13
//    l21 l22   0 *   0 u22 u23 = a21 a22 a23
//    l31 l32 l33     0   0 u33   a31 a32 a33
//  
//  Step 1:
//    l11 = 1
//    l22 = 1
//    l33 = 1
//
//  Step 2:
//    u11 = a11 / l11
//    l21 = a21 / u11
//    l31 = a31 / u11
//
//    u12 = a12 / l11
//    u22 = (a22 - u12 * l21) / l22
//    l32 = (a32 - l31 * u12) / u22
//
//    u13 = a13 / l11
//    u23 = (a23 - u13 * l21) / l22
//    u33 = (a33 - u13 * l31 - u23 * l32) / l33
//
//  In general, from above we can derive the following equations for solving the specific elements:
//  l[i][i] = 1
//  
//  u[i][j] = 1 / l[i][i] * (a[i][j] - <l[0 .. (i - 1)][:], u[:][0 .. (i - 1)]>)
//          = a[i][j] - <l[0 .. (i - 1)][:], u[:][0 .. (i - 1)]>                  , (as l[i][i] = 1).
//  
//  l[i][j] = 1 / u[j][j] * (a[i][j] - <l[0 .. (j - 1)][:], u[:][0 .. (j - 1)]>)
//
//  The above algorithm is generally known as Doolittle's method.
//
//  Examining the above equations closely, we can determine that the algorithm works as long as u[j][j]
//  is not zero (which would cause a division by zero). We can generalize this by adding a permutation
//  matrix P, giving us LUP decomposition, which always exists: (Doolittle's method with pivoting).
//
//  PA = LU,
//    where P is a permutation matrix,
//    L is a lower triangular matrix
//    and U is an upper triangular matrix.
//
//  Applying pivoting also makes the algorithm numerically stable. We can change the algorithm to do
//  (partial) pivoting in the following way:
//    1. When calculating l[i][j], do not perform the division with u[j][j].
//    2. Once you have calculated all the values for the column j, find the maximum value of lu[i..(n-1)][j].
//    3. Swap the pivot row with the row with the maximum value, so that the maximum value is at the pivot row.
//    4. Perform the division steps of the original algorithm with the new value of u[j][j] (the maximum).
//
//  LUP decomposition can be used to solve a set of linear equations 'Ax = b' in the following manner
//  (assuming A is invertible):
//    Ax = b
//    PAx = Pb
//    LUx = Pb
//    (LU)^-1 LUx = (LU)^-1 Pb
//    x = (LU)^-1 Pb
//      = U^-1 L^-1 Pb
//
pub struct LUDecomposition<T> {
  // U is stored in diagonals and above. Elements below diagonals are zero for U.
  // L is stored below diagonals. Diagonal elements are one for U and elements above diagonals are zero.
  lu : Matrix<T>,
  pospivsign : bool,
  piv : Vec<usize>
}

impl<T : Float + ApproxEq<T> + Signed> LUDecomposition<T> {
  pub fn new(a : &Matrix<T>) -> LUDecomposition<T> {
    let mut ludata = a.get_data().clone();
    let m = a.rows();
    let n = a.cols();
    let mut pivdata = alloc_dirty_vec(m);
    for i in 0..m {
      pivdata[i] = i;
    }

    let mut pospivsign = true;

    // Solve columns 0..(n-1) of L and U in order.
    for j in 0..n {
      // Solve column j of L and U.

      // Note that apart from the division, both L and U elements are calculated the same way,
      // so we only need one loop:
      // lu[i][j] = a[i][j] - <l[0 .. (j - 1)][:], u[:][0 .. (j - 1)]>
      for i in 0..m {
        let mut s : T = num::zero();
        for k in 0..cmp::min(i, j) {
          s = s + ludata[i * n + k] * ludata[k * n + j];
        }

        ludata[i * n + j] = ludata[i * n + j] - s;
      }

      // Find row with maximum pivot element at or below the diagonal.
      let mut p = j;
      for i in (j + 1)..m {
        if num::abs(ludata[i * n + j]) > num::abs(ludata[p * n + j]) {
          p = i;
        }
      }

      // Swap pivot row with the maximum row (unless pivot row is the maximum row already).
      if p != j {
        for k in 0..n {
          let t = ludata[p * n + k];
          ludata[p * n + k] = ludata[j * n + k];
          ludata[j * n + k] = t;
        }

        let k = pivdata[p as usize];
        pivdata[p as usize] = pivdata[j];
        pivdata[j as usize] = k;

        pospivsign = !pospivsign;
      }

      // Complete calculating the elements of the column of L:
      //  l[i][j] := 1 / u[j][j] * l[i][j]
      if (j < m) && (ludata[j * n + j] != num::zero()) {
        for i in (j + 1)..m {
          ludata[i * n + j] = ludata[i * n + j] / ludata[j * n + j];
        }
      }
    }

    LUDecomposition { 
      lu : Matrix::new(m as usize, n as usize, ludata),
      pospivsign : pospivsign,
      piv : pivdata
    }
  }

  pub fn is_singular(&self) -> bool {
    !self.is_non_singular()
  }

  pub fn is_non_singular(&self) -> bool {
    let n = self.lu.cols();
    for j in 0..n {
      if self.lu.get_data()[j * n + j] == num::zero() {
        return false;
      }
    }
    true
  }

  pub fn get_l(&self) -> Matrix<T> {
    // L is stored below diagonals. Diagonal elements are one for U and elements above diagonals are zero.
    let m = self.lu.rows();
    let n = if self.lu.rows() >= self.lu.cols() { self.lu.cols() } else { self.lu.rows() };
    let mut ldata = alloc_dirty_vec(m * n);
    for i in 0..m {
      for j in 0..n {
        ldata[i * n + j] =
            if i > j {
              self.lu.get_data()[i * self.lu.cols() + j]
            } else if i == j {
              num::one()
            } else {
              num::zero()
            }
      }
    }
    Matrix::new(m, n, ldata)
  }

  pub fn get_u(&self) -> Matrix<T> {
    // U is stored in diagonals and above. Elements below diagonals are zero for U.
    let m = if self.lu.rows() >= self.lu.cols() { self.lu.cols() } else { self.lu.rows() };
    let n = self.lu.cols();
    let mut udata = alloc_dirty_vec(m * n);
    for i in 0..m {
      for j in 0..n {
        udata[i * n + j] = if i <= j { self.lu.get_data()[i * n + j] } else { num::zero() };
      }
    }
    Matrix::new(m as usize, n as usize, udata)
  }

  pub fn get_p(&self) -> Matrix<T> {
    let len = self.piv.len();
    Matrix::id(len, len).permute_rows(&self.piv)
  }

  pub fn get_piv<'lt>(&'lt self) -> &'lt Vec<usize> { &self.piv }

  pub fn det(&self) -> T {
    assert!(self.lu.rows() == self.lu.cols());
    let n = self.lu.cols();
    let mut d = if self.pospivsign { num::one::<T>() } else { - num::one::<T>() };
    for j in 0..n {
      d = d * self.lu.get_data()[j * n + j];
    }
    d
  }

  // Solve A*X = B
  // B   A Matrix with as many rows as A and any number of columns.
  // Returns X so that L*U*X = B(piv,:)
  pub fn solve(&self, b : &Matrix<T>) -> Option<Matrix<T>> {
    let m = self.lu.rows();
    let n = self.lu.cols();
    assert!(b.rows() == m as usize);
    if !self.is_non_singular() {
      return None
    }

    // Copy right hand side with pivoting
    let nx = b.cols();
    let mut xdata = alloc_dirty_vec((m * nx) as usize);
    let mut dest_idx = 0;
    for i in 0..self.piv.len() {
      for j in 0..nx {
        xdata[dest_idx] = b.get_data()[self.piv[i] * b.cols() + j];
        dest_idx += 1;
      }
    }

    // Solve L*Y = B(piv,:)
    for k in 0..n {
      for i in (k + 1)..n {
        for j in 0..nx {
          xdata[i * nx + j] = xdata[i * nx + j] - xdata[k * nx + j] * self.lu.get_data()[i * self.lu.cols() + k];
        }
      }
    }

    // Solve U*X = Y;
    for k in (0..n).rev() {
      for j in 0..nx {
        xdata[k * nx + j] = xdata[k * nx + j] / self.lu.get_data()[k * self.lu.cols() + k];
      }
      for i in 0..k {
        for j in 0..nx {
          xdata[i * nx + j] = xdata[i * nx + j] - xdata[k * nx + j] * self.lu.get_data()[i * self.lu.cols() + k];
        }
      }
    }

    Some(Matrix::new(self.lu.rows(), b.cols(), xdata))
  }
}

#[test]
fn test_lu_square() {
  let a = m!(1.0, 2.0, 0.0; 3.0, 6.0, -1.0; 1.0, 2.0, 1.0);
  let lu = LUDecomposition::new(&a);
  let l = lu.get_l();
  let u = lu.get_u();
  let p = lu.get_p();
  assert!(l * u == p * a);
}

#[test]
fn test_lu2_m_over_n() {
  let a = m!(1.0, 2.0; 3.0, 4.0; 5.0, 6.0);
  let lu = LUDecomposition::new(&a);
  let l = lu.get_l();
  let u = lu.get_u();
  let p = lu.get_p();
  assert!(l * u == p * a);
}

#[test]
fn test_lu2_m_under_n() {
  let a = m!(1.0, 2.0, 3.0; 4.0, 5.0, 6.0);
  let lu = LUDecomposition::new(&a);
  let l = lu.get_l();
  let u = lu.get_u();
  let p = lu.get_p();
  assert!(l * u == p * a);
}

#[test]
fn lu_solve_test() {
  let a = m!(2.0, 1.0, 0.0; 1.0, 1.0, 0.0; 0.0, 0.0, 1.0);
  let lu = LUDecomposition::new(&a);
  let b = m!(1.0; 2.0; 3.0);
  assert!(lu.solve(&b).unwrap().approx_eq(&m!(-1.0; 3.0; 3.0)));
}

#[test]
#[should_panic]
fn lu_solve_test_incompatible() {
  let a = m!(2.0, 1.0, 0.0; 1.0, 1.0, 0.0; 0.0, 0.0, 1.0);
  let lu = LUDecomposition::new(&a);
  let b = m!(1.0; 2.0; 3.0; 4.0);
  let _ = lu.solve(&b);
}

#[test]
fn lu_solve_test_singular() {
  let a = m!(2.0, 6.0; 1.0, 3.0);
  let lu = LUDecomposition::new(&a);
  let b = m!(1.0; 2.0);
  assert!(lu.solve(&b).is_none());
}

#[test]
fn lu_is_singular_test() {
  let a = m!(2.0, 6.0; 1.0, 3.0);
  let lu = LUDecomposition::new(&a);
  assert!(lu.is_singular());

  let a = m!(2.0, 6.0; 1.0, 4.0);
  let lu = LUDecomposition::new(&a);
  assert!(!lu.is_singular());
}

#[test]
fn lu_is_non_singular_test() {
  let a = m!(4.0, 8.0; 3.0, 4.0);
  let lu = LUDecomposition::new(&a);
  assert!(lu.is_non_singular());

  let a = m!(4.0, 6.0; 2.0, 3.0);
  let lu = LUDecomposition::new(&a);
  assert!(!lu.is_non_singular());
}

#[test]
fn lu_det_test() {
  let a = m!(4.0, 8.0; 3.0, 4.0);
  let lu = LUDecomposition::new(&a);
  assert!(lu.det() == -8.0);

  let a = m!(4.0, 8.0; 2.0, 4.0);
  let lu = LUDecomposition::new(&a);
  assert!(lu.det() == 0.0);
}

#[test]
#[should_panic]
fn lu_det_test_not_square() {
  let a = m!(1.0, 2.0, 3.0; 4.0, 5.0, 6.0);
  let lu = LUDecomposition::new(&a);
  let _ = lu.det();
}
