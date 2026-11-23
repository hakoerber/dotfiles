pub enum TreeNode<'a, K, T> {
    Node((K, &'a [Self])),
    Leaf((K, T)),
}

pub struct TreeRoot<'a, K, T>(pub &'a [TreeNode<'a, K, T>]);

impl<'a, K: Clone, T> IntoIterator for TreeRoot<'a, K, T> {
    type Item = (Vec<K>, &'a T);

    type IntoIter = std::vec::IntoIter<(Vec<K>, &'a T)>;

    fn into_iter(self) -> Self::IntoIter {
        fn handle_node<'a, K: Clone, T>(
            path: &[K],
            node: &'a TreeNode<K, T>,
        ) -> Vec<(Vec<K>, &'a T)> {
            let mut v = vec![];
            match *node {
                TreeNode::Node((ref subpath, subnodes)) => {
                    let mut path = path.to_owned();
                    path.push(subpath.clone());
                    for node in subnodes {
                        v.extend(handle_node(&path.clone(), node));
                    }
                }
                TreeNode::Leaf((ref subpath, ref elem)) => {
                    let mut path = path.to_owned();
                    path.push(subpath.clone());
                    v.push((path, elem));
                }
            }
            v
        }

        let mut v = vec![];
        for node in self.0 {
            v.extend(handle_node(&[], node));
        }
        v.into_iter()
    }
}
