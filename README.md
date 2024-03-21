# kubectl-topology-skew

kubectl plugin to display pod count and skew per topology.

![test](https://github.com/watawuwu/kubectl-topology-skew/workflows/Test/badge.svg)
[![codecov](https://codecov.io/gh/watawuwu/kubectl-topology-skew/branch/main/graph/badge.svg)](https://codecov.io/gh/watawuwu/kubectl-topology-skew)
![License](https://img.shields.io/github/license/watawuwu/kubectl-topology-skew)

## Getting Started

The `kubectl-topology-skew` command displays the number and skew of Pods placed in the topology for each resource, such as deployment, daemonset, statefulset, and job.

```
 ❯❯ kubectl topology-skew -h
kubectl plugin to display the count of pods and nodes per topology

Usage: kubectl-topology_skew [OPTIONS] <COMMAND>

Commands:
  pod          Print pod topology skew [aliases: po]
  deployment   Print deployment topology skew [aliases: deploy]
  statefulset  Print statefulset topology skew [aliases: sts]
  daemonset    Print daemonset topology skew [aliases: ds]
  job          Print daemonset topology skew
  all          Print topology skew of resources such as deploy, sts, ds, jobs, etc
  node         Print node topology skew [aliases: no]
  help         Print this message or the help of the given subcommand(s)

Options:
      --context <CONTEXT>  Kubernetes config context
      --cluster <CLUSTER>  Kubernetes config cluster
      --user <USER>        Kubernetes config user
  -o, --output <OUTPUT>    Output format [default: text] [possible values: text, yaml, json]
  -h, --help               Print help
  -V, --version            Print version
```

The value displayed in the `skew` column is calculated using the formula described in [this blog](https://kubernetes.io/blog/2020/05/introducing-podtopologyspread/#api-changes).

`skew = Pods number matched in current topology - min Pods matches in a topology`.

```
 ❯❯ kubectl topology-skew deploy

       apps/v1/deployment/one
────────────────────────────────────
  TOPOLOGY            COUNT   SKEW
  asia-northeast1-a   1       1
  asia-northeast1-b   0       0
  asia-northeast1-c   0       0

      apps/v1/deployment/nginx
────────────────────────────────────
  TOPOLOGY            COUNT   SKEW
  asia-northeast1-a   3       0
  asia-northeast1-b   4       1
  asia-northeast1-c   4       1

    apps/v1/deployment/unbalance
────────────────────────────────────
  TOPOLOGY            COUNT   SKEW
  asia-northeast1-a   5       0
  asia-northeast1-b   9       4
  asia-northeast1-c   6       1
```

The default topology key is `topology.kubernetes.io/zone`, but you can specify any label set on the nodes using the optional `--topology-key(-t)`.

Additionally, in pod resources, the selector option is available, so you can use it when you want to display Pods belonging to custom resources or multiple resources.

```
 ❯❯ kubectl topology-skew pod -h
Print pod topology skew

Usage: kubectl-topology_skew pod [OPTIONS]

Options:
      --context <CONTEXT>            Kubernetes config context
  -n, --namespace <NAMESPACE>        Kubernetes namespace name
      --cluster <CLUSTER>            Kubernetes config cluster
  -t, --topology-key <TOPOLOGY_KEY>  Topology key [default: topology.kubernetes.io/zone]
  -l, --selector <SELECTOR>          Label selector for pod list
      --user <USER>                  Kubernetes config user
  -o, --output <OUTPUT>              Output format [default: text] [possible values: text, yaml, json]
  -h, --help                         Print help
```

### Installing

```
$ brew install watawuwu/homebrew-tap/kubectl-topology-skew
```

## Contributing

Please read [CONTRIBUTING.md](https://gist.github.com/PurpleBooth/b24679402957c63ec426) for details on our code of conduct, and the process for submitting pull requests to us.

## Versioning

We use [SemVer](http://semver.org/) for versioning.

## License
This project is licensed under either of

- Apache License, Version 2.0, (http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (http://opensource.org/licenses/MIT)

at your option.

## Authors

* [Wataru Matsui](watawuwu@3bi.tech)
